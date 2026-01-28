/**
 * Data Table Component
 *
 * Reusable data table functionality for admin panel tables.
 * Features: row selection, column visibility, search debouncing,
 * filter state serialization, HTMX integration.
 */

/**
 * DataTable class for managing table state and interactions.
 */
class DataTable {
    /**
     * Create a DataTable instance.
     *
     * @param {string} tableId - The unique identifier for this table (e.g., "customers")
     * @param {Object} options - Configuration options
     * @param {string[]} options.defaultColumns - Default visible columns
     * @param {number} options.searchDebounceMs - Search input debounce delay (default: 300)
     */
    constructor(tableId, options = {}) {
        this.tableId = tableId;
        this.selectedRows = new Set();
        this.searchDebounceMs = options.searchDebounceMs || 300;
        this.defaultColumns = options.defaultColumns || [];
        this.visibleColumns = new Set(this.defaultColumns);
        this.searchTimeout = null;

        this.init();
    }

    /**
     * Initialize the data table.
     */
    init() {
        this.loadPreferences();
        this.bindEvents();
        this.updateColumnVisibility();
        this.updateBulkActionBar();
    }

    /**
     * Load user preferences from localStorage and server.
     */
    loadPreferences() {
        const stored = localStorage.getItem(`table.${this.tableId}.columns`);
        if (stored) {
            try {
                const columns = JSON.parse(stored);
                this.visibleColumns = new Set(columns);
            } catch (e) {
                console.error('Failed to parse stored columns:', e);
            }
        }
    }

    /**
     * Save user preferences to localStorage and server.
     */
    async savePreferences() {
        const columns = Array.from(this.visibleColumns);
        localStorage.setItem(`table.${this.tableId}.columns`, JSON.stringify(columns));

        // Save to server for cross-device sync
        try {
            await fetch(`/api/preferences/table/${this.tableId}`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ columns }),
            });
        } catch (e) {
            console.error('Failed to save preferences to server:', e);
        }
    }

    /**
     * Bind event listeners.
     */
    bindEvents() {
        // Select all checkbox
        const selectAll = document.getElementById(`${this.tableId}-select-all`);
        if (selectAll) {
            selectAll.addEventListener('change', (e) => this.handleSelectAll(e));
        }

        // Individual row checkboxes
        document.querySelectorAll(`[data-table="${this.tableId}"] input[type="checkbox"][data-row-id]`)
            .forEach(checkbox => {
                checkbox.addEventListener('change', (e) => this.handleRowSelect(e));
            });

        // Search input
        const searchInput = document.getElementById(`${this.tableId}-search`);
        if (searchInput) {
            searchInput.addEventListener('input', (e) => this.handleSearch(e));
        }

        // Column visibility toggles
        document.querySelectorAll(`[data-column-toggle="${this.tableId}"]`)
            .forEach(toggle => {
                toggle.addEventListener('change', (e) => this.handleColumnToggle(e));
            });

        // Filter inputs
        document.querySelectorAll(`[data-filter="${this.tableId}"]`)
            .forEach(filter => {
                filter.addEventListener('change', (e) => this.handleFilterChange(e));
            });

        // Bulk action buttons
        document.querySelectorAll(`[data-bulk-action="${this.tableId}"]`)
            .forEach(button => {
                button.addEventListener('click', (e) => this.handleBulkAction(e));
            });

        // Listen for HTMX events to rebind after content swap
        document.body.addEventListener('htmx:afterSwap', (e) => {
            if (e.detail.target.closest(`[data-table="${this.tableId}"]`)) {
                this.rebindRowCheckboxes();
                this.updateColumnVisibility();
            }
        });
    }

    /**
     * Rebind row checkboxes after HTMX content swap.
     */
    rebindRowCheckboxes() {
        document.querySelectorAll(`[data-table="${this.tableId}"] input[type="checkbox"][data-row-id]`)
            .forEach(checkbox => {
                checkbox.removeEventListener('change', this.handleRowSelect);
                checkbox.addEventListener('change', (e) => this.handleRowSelect(e));

                // Restore selection state
                const rowId = checkbox.dataset.rowId;
                checkbox.checked = this.selectedRows.has(rowId);
            });

        this.updateSelectAllState();
    }

    /**
     * Handle select all checkbox change.
     *
     * @param {Event} e - Change event
     */
    handleSelectAll(e) {
        const checked = e.target.checked;
        document.querySelectorAll(`[data-table="${this.tableId}"] input[type="checkbox"][data-row-id]`)
            .forEach(checkbox => {
                checkbox.checked = checked;
                const rowId = checkbox.dataset.rowId;
                if (checked) {
                    this.selectedRows.add(rowId);
                } else {
                    this.selectedRows.delete(rowId);
                }
            });

        this.updateBulkActionBar();
    }

    /**
     * Handle individual row checkbox change.
     *
     * @param {Event} e - Change event
     */
    handleRowSelect(e) {
        const rowId = e.target.dataset.rowId;
        if (e.target.checked) {
            this.selectedRows.add(rowId);
        } else {
            this.selectedRows.delete(rowId);
        }

        this.updateSelectAllState();
        this.updateBulkActionBar();
    }

    /**
     * Update the select all checkbox state based on row selections.
     */
    updateSelectAllState() {
        const selectAll = document.getElementById(`${this.tableId}-select-all`);
        if (!selectAll) return;

        const allCheckboxes = document.querySelectorAll(
            `[data-table="${this.tableId}"] input[type="checkbox"][data-row-id]`
        );
        const checkedCount = this.selectedRows.size;
        const totalCount = allCheckboxes.length;

        if (checkedCount === 0) {
            selectAll.checked = false;
            selectAll.indeterminate = false;
        } else if (checkedCount === totalCount) {
            selectAll.checked = true;
            selectAll.indeterminate = false;
        } else {
            selectAll.checked = false;
            selectAll.indeterminate = true;
        }
    }

    /**
     * Update bulk action bar visibility and selected count.
     */
    updateBulkActionBar() {
        const bulkBar = document.getElementById(`${this.tableId}-bulk-bar`);
        if (!bulkBar) return;

        const count = this.selectedRows.size;
        if (count > 0) {
            bulkBar.classList.remove('hidden');
            const countSpan = bulkBar.querySelector('[data-selected-count]');
            if (countSpan) {
                countSpan.textContent = `${count} selected`;
            }
        } else {
            bulkBar.classList.add('hidden');
        }
    }

    /**
     * Handle search input with debouncing.
     *
     * @param {Event} e - Input event
     */
    handleSearch(e) {
        const query = e.target.value;

        if (this.searchTimeout) {
            clearTimeout(this.searchTimeout);
        }

        this.searchTimeout = setTimeout(() => {
            this.updateUrlAndRefresh({ query: query || null });
        }, this.searchDebounceMs);
    }

    /**
     * Handle column visibility toggle.
     *
     * @param {Event} e - Change event
     */
    handleColumnToggle(e) {
        const column = e.target.dataset.column;
        if (e.target.checked) {
            this.visibleColumns.add(column);
        } else {
            this.visibleColumns.delete(column);
        }

        this.updateColumnVisibility();
        this.savePreferences();
    }

    /**
     * Update column visibility in the table.
     */
    updateColumnVisibility() {
        const table = document.querySelector(`[data-table="${this.tableId}"]`);
        if (!table) return;

        // Get all column headers and cells with data-column attribute
        const headers = table.querySelectorAll('th[data-column]');
        headers.forEach(th => {
            const column = th.dataset.column;
            const isVisible = this.visibleColumns.has(column);

            // Toggle header visibility
            th.classList.toggle('hidden', !isVisible);

            // Toggle corresponding cells by matching data-column attribute
            table.querySelectorAll(`td[data-column="${column}"]`).forEach(cell => {
                cell.classList.toggle('hidden', !isVisible);
            });
        });

        // Update column picker checkboxes
        document.querySelectorAll(`[data-column-toggle="${this.tableId}"]`)
            .forEach(toggle => {
                toggle.checked = this.visibleColumns.has(toggle.dataset.column);
            });
    }

    /**
     * Handle filter change.
     *
     * @param {Event} e - Change event
     */
    handleFilterChange(e) {
        const filterName = e.target.name;
        const filterValue = e.target.value;

        this.updateUrlAndRefresh({ [filterName]: filterValue || null });
    }

    /**
     * Update URL parameters and refresh table content.
     *
     * @param {Object} params - Parameters to update
     */
    updateUrlAndRefresh(params) {
        const url = new URL(window.location.href);

        Object.entries(params).forEach(([key, value]) => {
            if (value === null || value === '') {
                url.searchParams.delete(key);
            } else {
                url.searchParams.set(key, value);
            }
        });

        // Remove cursor when filters change
        if (!params.hasOwnProperty('cursor')) {
            url.searchParams.delete('cursor');
        }

        // Update URL without reload
        window.history.pushState({}, '', url.toString());

        // Trigger HTMX refresh
        const table = document.querySelector(`[data-table="${this.tableId}"]`);
        if (table) {
            htmx.ajax('GET', url.pathname + url.search, {
                target: table,
                swap: 'innerHTML',
            });
        }
    }

    /**
     * Handle bulk action button click.
     *
     * @param {Event} e - Click event
     */
    handleBulkAction(e) {
        const action = e.target.dataset.action;
        const ids = Array.from(this.selectedRows);

        if (ids.length === 0) {
            return;
        }

        // Dispatch custom event for handling specific actions
        const event = new CustomEvent('dataTableBulkAction', {
            detail: {
                tableId: this.tableId,
                action,
                ids,
            },
        });
        document.dispatchEvent(event);
    }

    /**
     * Clear all selections.
     */
    clearSelection() {
        this.selectedRows.clear();

        document.querySelectorAll(`[data-table="${this.tableId}"] input[type="checkbox"]`)
            .forEach(checkbox => {
                checkbox.checked = false;
            });

        this.updateBulkActionBar();
    }

    /**
     * Get selected row IDs.
     *
     * @returns {string[]} Array of selected row IDs
     */
    getSelectedIds() {
        return Array.from(this.selectedRows);
    }

    /**
     * Set visible columns programmatically.
     *
     * @param {string[]} columns - Array of column names to show
     */
    setVisibleColumns(columns) {
        this.visibleColumns = new Set(columns);
        this.updateColumnVisibility();
        this.savePreferences();
    }

    /**
     * Get current filter state from URL.
     *
     * @returns {Object} Filter parameters
     */
    getFilters() {
        const url = new URL(window.location.href);
        const filters = {};
        url.searchParams.forEach((value, key) => {
            filters[key] = value;
        });
        return filters;
    }

    /**
     * Set filters and refresh.
     *
     * @param {Object} filters - Filter parameters
     */
    setFilters(filters) {
        this.updateUrlAndRefresh(filters);
    }

    /**
     * Clear all filters.
     */
    clearFilters() {
        const url = new URL(window.location.href);
        const preserveKeys = ['sort', 'dir']; // Preserve sort settings

        Array.from(url.searchParams.keys()).forEach(key => {
            if (!preserveKeys.includes(key)) {
                url.searchParams.delete(key);
            }
        });

        window.history.pushState({}, '', url.toString());

        const table = document.querySelector(`[data-table="${this.tableId}"]`);
        if (table) {
            htmx.ajax('GET', url.pathname + url.search, {
                target: table,
                swap: 'innerHTML',
            });
        }
    }
}

/**
 * Debounce utility function.
 *
 * @param {Function} func - Function to debounce
 * @param {number} wait - Delay in milliseconds
 * @returns {Function} Debounced function
 */
function debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}

/**
 * Initialize a data table with default options.
 *
 * @param {string} tableId - Table identifier
 * @param {Object} options - Options
 * @returns {DataTable} DataTable instance
 */
function initDataTable(tableId, options = {}) {
    return new DataTable(tableId, options);
}

// Store instances for access
const dataTableInstances = {};

/**
 * Get or create a DataTable instance.
 *
 * @param {string} tableId - Table identifier
 * @param {Object} options - Options (only used if creating new instance)
 * @returns {DataTable} DataTable instance
 */
function getDataTable(tableId, options = {}) {
    if (!dataTableInstances[tableId]) {
        dataTableInstances[tableId] = new DataTable(tableId, options);
    }
    return dataTableInstances[tableId];
}

// Export for use in other scripts
window.DataTable = {
    init: initDataTable,
    get: getDataTable,
    debounce,
    instances: dataTableInstances,
};

// Auto-initialize tables with data-table-auto attribute
document.addEventListener('DOMContentLoaded', () => {
    document.querySelectorAll('[data-table-auto]').forEach(table => {
        const tableId = table.dataset.table;
        const optionsStr = table.dataset.tableOptions;
        let options = {};

        if (optionsStr) {
            try {
                options = JSON.parse(optionsStr);
            } catch (e) {
                console.error('Failed to parse table options:', e);
            }
        }

        getDataTable(tableId, options);
    });
});
