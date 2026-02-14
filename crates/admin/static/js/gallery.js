/**
 * Gallery — drag-and-drop upload, multi-select, lightbox, metadata editing.
 */
(function () {
    'use strict';

    // =========================================================================
    // State
    // =========================================================================
    var selectedKeys = new Set();

    // =========================================================================
    // Upload
    // =========================================================================
    var dropZone = document.getElementById('gallery-drop-zone');
    var fileInput = document.getElementById('gallery-file-input');
    var progressContainer = document.getElementById('gallery-upload-progress');
    var progressBar = document.getElementById('gallery-upload-bar');
    var statusText = document.getElementById('gallery-upload-status');
    var imageGrid = document.getElementById('gallery-image-grid');

    if (dropZone && fileInput) {
        fileInput.addEventListener('change', function (e) {
            var files = Array.from(e.target.files);
            if (files.length > 0) uploadFiles(files);
            fileInput.value = '';
        });

        dropZone.addEventListener('dragover', function (e) {
            e.preventDefault();
            dropZone.classList.add('border-coral', 'bg-coral/5');
        });

        dropZone.addEventListener('dragleave', function (e) {
            e.preventDefault();
            dropZone.classList.remove('border-coral', 'bg-coral/5');
        });

        dropZone.addEventListener('drop', function (e) {
            e.preventDefault();
            dropZone.classList.remove('border-coral', 'bg-coral/5');
            var files = Array.from(e.dataTransfer.files).filter(function (f) {
                return f.type.startsWith('image/');
            });
            if (files.length > 0) uploadFiles(files);
        });
    }

    function uploadFiles(files) {
        var prefix = imageGrid ? (imageGrid.dataset.prefix || '') : '';

        progressContainer.classList.remove('hidden');
        var totalFiles = files.length;
        var completedFiles = 0;

        var formData = new FormData();
        formData.append('folder', prefix);
        files.forEach(function (f) {
            formData.append('files', f);
        });

        var xhr = new XMLHttpRequest();
        xhr.open('POST', '/gallery/upload');

        xhr.upload.addEventListener('progress', function (e) {
            if (e.lengthComputable) {
                var pct = Math.round((e.loaded / e.total) * 100);
                progressBar.style.width = pct + '%';
                statusText.textContent = 'Uploading... ' + pct + '%';
            }
        });

        xhr.addEventListener('load', function () {
            if (xhr.status >= 200 && xhr.status < 300) {
                statusText.textContent = 'Upload complete! Refreshing...';
                progressBar.style.width = '100%';
                setTimeout(function () { window.location.reload(); }, 800);
            } else {
                statusText.textContent = 'Upload failed.';
                setTimeout(function () {
                    progressContainer.classList.add('hidden');
                }, 2000);
            }
        });

        xhr.addEventListener('error', function () {
            statusText.textContent = 'Upload failed.';
            setTimeout(function () {
                progressContainer.classList.add('hidden');
            }, 2000);
        });

        xhr.send(formData);
    }

    // =========================================================================
    // Multi-select
    // =========================================================================
    var bulkBar = document.getElementById('gallery-bulk-bar');
    var selectedCountEl = document.getElementById('gallery-selected-count');
    var lastCheckedIndex = -1;

    document.addEventListener('change', function (e) {
        if (!e.target.classList.contains('gallery-checkbox')) return;
        var key = e.target.dataset.key;

        if (e.target.checked) {
            selectedKeys.add(key);
        } else {
            selectedKeys.delete(key);
        }

        // Shift-click range select
        if (e.shiftKey && lastCheckedIndex >= 0) {
            var checkboxes = Array.from(document.querySelectorAll('.gallery-checkbox'));
            var currentIndex = checkboxes.indexOf(e.target);
            var start = Math.min(lastCheckedIndex, currentIndex);
            var end = Math.max(lastCheckedIndex, currentIndex);
            for (var i = start; i <= end; i++) {
                checkboxes[i].checked = true;
                selectedKeys.add(checkboxes[i].dataset.key);
            }
        }

        lastCheckedIndex = Array.from(document.querySelectorAll('.gallery-checkbox')).indexOf(e.target);
        updateBulkBar();
    });

    function updateBulkBar() {
        if (!bulkBar) return;
        if (selectedKeys.size > 0) {
            bulkBar.classList.remove('hidden');
            selectedCountEl.textContent = selectedKeys.size;
        } else {
            bulkBar.classList.add('hidden');
        }
    }

    window.galleryClearSelection = function () {
        selectedKeys.clear();
        document.querySelectorAll('.gallery-checkbox').forEach(function (cb) {
            cb.checked = false;
        });
        updateBulkBar();
    };

    window.galleryBulkDelete = function () {
        if (selectedKeys.size === 0) return;
        if (!confirm('Delete ' + selectedKeys.size + ' image(s)? This cannot be undone.')) return;

        var keys = Array.from(selectedKeys);
        fetch('/gallery/bulk-delete', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ keys: keys })
        }).then(function (resp) {
            if (resp.ok) {
                // Remove cards from grid
                keys.forEach(function (key) {
                    var card = document.querySelector('[data-key="' + CSS.escape(key) + '"]');
                    if (card) card.remove();
                });
                selectedKeys.clear();
                updateBulkBar();
                showToast('Images deleted', 'success');
            } else {
                showToast('Delete failed', 'error');
            }
        }).catch(function () {
            showToast('Delete failed', 'error');
        });
    };

    // =========================================================================
    // Single delete
    // =========================================================================
    window.galleryDeleteSingle = function (key) {
        if (!confirm('Delete this image? This cannot be undone.')) return;

        var formData = new FormData();
        formData.append('key', key);

        fetch('/gallery/delete', {
            method: 'POST',
            body: formData
        }).then(function (resp) {
            if (resp.ok) {
                var card = document.querySelector('[data-key="' + CSS.escape(key) + '"]');
                if (card) card.remove();
                showToast('Image deleted', 'success');
            } else {
                showToast('Delete failed', 'error');
            }
        }).catch(function () {
            showToast('Delete failed', 'error');
        });
    };

    // =========================================================================
    // Lightbox
    // =========================================================================
    var lightbox = document.getElementById('gallery-lightbox');
    var lightboxContent = document.getElementById('gallery-lightbox-content');

    window.galleryOpenLightbox = function (key) {
        if (!lightbox || !lightboxContent) return;
        lightbox.classList.remove('hidden');
        document.body.style.overflow = 'hidden';

        lightboxContent.innerHTML = '<div class="p-12 text-center"><div class="animate-spin inline-block"><i class="ph ph-spinner text-coral text-2xl"></i></div></div>';

        fetch('/gallery/image/' + encodeURIComponent(key), {
            headers: { 'HX-Request': 'true' }
        }).then(function (resp) {
            return resp.text();
        }).then(function (html) {
            lightboxContent.innerHTML = html;
            // Execute inline scripts
            lightboxContent.querySelectorAll('script').forEach(function (script) {
                var newScript = document.createElement('script');
                newScript.textContent = script.textContent;
                script.parentNode.replaceChild(newScript, script);
            });
        }).catch(function () {
            lightboxContent.innerHTML = '<div class="p-12 text-center text-red-400">Failed to load image details</div>';
        });
    };

    window.galleryCloseLightbox = function (event) {
        if (event && event.target !== lightbox) return;
        if (!lightbox) return;
        lightbox.classList.add('hidden');
        document.body.style.overflow = '';
        lightboxContent.innerHTML = '';
    };

    // Close argument-less version for the X button
    var originalClose = window.galleryCloseLightbox;
    window.galleryCloseLightbox = function (event) {
        if (arguments.length === 0 || (event && event.target === lightbox)) {
            if (lightbox) {
                lightbox.classList.add('hidden');
                document.body.style.overflow = '';
                lightboxContent.innerHTML = '';
            }
        }
    };

    // Keyboard: Escape closes lightbox
    document.addEventListener('keydown', function (e) {
        if (e.key === 'Escape' && lightbox && !lightbox.classList.contains('hidden')) {
            window.galleryCloseLightbox();
        }
    });

    // =========================================================================
    // Toast
    // =========================================================================
    function showToast(message, type) {
        var existing = document.getElementById('gallery-toast');
        if (existing) existing.remove();

        var toast = document.createElement('div');
        toast.id = 'gallery-toast';
        toast.className = 'fixed bottom-4 right-4 px-4 py-3 rounded-lg shadow-lg text-sm font-medium z-[60] ' +
            (type === 'error' ? 'bg-red-600 text-white' : 'bg-emerald-600 text-white');
        toast.textContent = message;
        document.body.appendChild(toast);

        setTimeout(function () { toast.remove(); }, 3000);
    }
})();
