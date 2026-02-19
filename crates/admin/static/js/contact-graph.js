/**
 * Contact Graph Visualization
 *
 * Interactive Cytoscape.js graph for viewing and editing contacts and
 * their relationships. Coordinates with HTMX for CRUD operations via
 * the HX-Trigger: graph-updated event.
 */
(function () {
  'use strict';

  // ---------------------------------------------------------------------------
  // Theme colors (Midnight Lagoon)
  // ---------------------------------------------------------------------------
  var colors = {
    coral: '#d63a2f',
    coralMuted: 'rgba(214, 58, 47, 0.3)',
    lagoon: '#1e4d6e',
    lagoonLight: '#2d6a8a',
    secondary: '#6b8fa3',
    secondaryMuted: 'rgba(107, 143, 163, 0.5)',
    honey: '#d4a14a',
    textLight: '#e0f0f8',
    textDark: '#1a2332',
    bgDark: '#0d1f2d',
    bgLight: '#f8fafb',
  };

  function isDark() {
    return document.documentElement.classList.contains('dark');
  }

  function textColor() {
    return isDark() ? colors.textLight : colors.textDark;
  }

  function bgColor() {
    return isDark() ? colors.bgDark : colors.bgLight;
  }

  // ---------------------------------------------------------------------------
  // Cytoscape stylesheet
  // ---------------------------------------------------------------------------
  function cyStyle() {
    var tc = textColor();
    var outlineColor = isDark() ? 'rgba(13, 31, 45, 0.8)' : 'rgba(255, 255, 255, 0.8)';

    return [
      {
        selector: 'node',
        style: {
          label: 'data(name)',
          color: tc,
          'font-family': "'DM Sans', sans-serif",
          'font-size': '11px',
          'text-outline-color': outlineColor,
          'text-outline-width': 2,
          'text-valign': 'bottom',
          'text-margin-y': 6,
          'min-zoomed-font-size': 8,
          width: 36,
          height: 36,
          'border-width': 2,
          'border-opacity': 0.6,
        },
      },
      {
        selector: 'node[contact_type="person"]',
        style: {
          shape: 'ellipse',
          'background-color': colors.coral,
          'border-color': colors.coral,
        },
      },
      {
        selector: 'node[contact_type="organization"]',
        style: {
          shape: 'round-rectangle',
          'background-color': colors.lagoon,
          'border-color': colors.lagoonLight,
          width: 44,
          height: 32,
        },
      },
      {
        selector: 'edge',
        style: {
          width: 2,
          'line-color': colors.secondary,
          'target-arrow-color': colors.secondary,
          'target-arrow-shape': 'triangle',
          'arrow-scale': 0.8,
          'curve-style': 'bezier',
          label: 'data(relationship_type)',
          'font-family': "'DM Sans', sans-serif",
          'font-size': '9px',
          color: colors.secondaryMuted,
          'text-rotation': 'autorotate',
          'text-outline-color': outlineColor,
          'text-outline-width': 1.5,
          'min-zoomed-font-size': 8,
        },
      },
      {
        selector: ':selected',
        style: {
          'border-width': 3,
          'border-color': colors.coral,
          'line-color': colors.coral,
          'target-arrow-color': colors.coral,
        },
      },
      {
        selector: '.highlighted',
        style: {
          'border-width': 3,
          'border-color': colors.honey,
          'line-color': colors.honey,
          'target-arrow-color': colors.honey,
        },
      },
      {
        selector: '.dimmed',
        style: {
          opacity: 0.25,
        },
      },
    ];
  }

  // ---------------------------------------------------------------------------
  // Initialize Cytoscape
  // ---------------------------------------------------------------------------
  var cy = null;

  function initCy() {
    var container = document.getElementById('cy');
    if (!container) return;

    cy = cytoscape({
      container: container,
      style: cyStyle(),
      layout: { name: 'grid' },
      minZoom: 0.2,
      maxZoom: 4,
      wheelSensitivity: 0.3,
    });

    // Node click -> load contact detail
    cy.on('tap', 'node', function (evt) {
      var node = evt.target;
      var id = node.data('id');
      htmx.ajax('GET', '/contacts/' + id, {
        target: '#contact-detail-panel',
        swap: 'innerHTML',
      });
    });

    // Edge click -> load relationship detail
    cy.on('tap', 'edge', function (evt) {
      var edge = evt.target;
      var id = edge.data('db_id');
      if (id) {
        htmx.ajax('GET', '/contacts/relationships/' + id, {
          target: '#contact-detail-panel',
          swap: 'innerHTML',
        });
      }
    });

    // Background click -> reset detail panel
    cy.on('tap', function (evt) {
      if (evt.target === cy) {
        clearSearch();
      }
    });

    fetchGraph();
  }

  // ---------------------------------------------------------------------------
  // Data loading
  // ---------------------------------------------------------------------------
  function fetchGraph() {
    var loading = document.getElementById('graph-loading');
    var empty = document.getElementById('graph-empty');
    if (loading) loading.classList.remove('hidden');
    if (empty) empty.classList.add('hidden');

    fetch('/contacts/api/graph', { credentials: 'same-origin' })
      .then(function (res) { return res.json(); })
      .then(function (data) {
        renderGraph(data);
      })
      .catch(function (err) {
        console.error('Failed to fetch graph:', err);
      })
      .finally(function () {
        if (loading) loading.classList.add('hidden');
      });
  }

  function renderGraph(data) {
    if (!cy) return;

    var empty = document.getElementById('graph-empty');

    if (data.nodes.length === 0) {
      cy.elements().remove();
      if (empty) empty.classList.remove('hidden');
      return;
    }
    if (empty) empty.classList.add('hidden');

    // Build Cytoscape elements
    var elements = [];

    data.nodes.forEach(function (node) {
      elements.push({
        group: 'nodes',
        data: {
          id: String(node.id),
          name: node.name,
          contact_type: node.contact_type,
          email: node.email || '',
          domain: node.domain || '',
        },
      });
    });

    data.edges.forEach(function (edge) {
      elements.push({
        group: 'edges',
        data: {
          id: 'e' + edge.id,
          db_id: edge.id,
          source: String(edge.from_contact_id),
          target: String(edge.to_contact_id),
          relationship_type: edge.relationship_type,
        },
      });
    });

    // Replace all elements and run layout
    cy.json({ elements: elements });
    cy.style(cyStyle());

    runLayout();
  }

  function runLayout() {
    if (!cy || cy.nodes().length === 0) return;

    cy.layout({
      name: 'cose',
      animate: true,
      animationDuration: 500,
      nodeRepulsion: function () { return 8000; },
      idealEdgeLength: function () { return 120; },
      edgeElasticity: function () { return 100; },
      gravity: 0.25,
      numIter: 200,
      padding: 40,
      randomize: false,
      fit: true,
    }).run();
  }

  // ---------------------------------------------------------------------------
  // Search / filter
  // ---------------------------------------------------------------------------
  function setupSearch() {
    var input = document.getElementById('graph-search');
    if (!input) return;

    var debounceTimer;
    input.addEventListener('input', function () {
      clearTimeout(debounceTimer);
      debounceTimer = setTimeout(function () {
        filterNodes(input.value.trim().toLowerCase());
      }, 200);
    });
  }

  function filterNodes(query) {
    if (!cy) return;

    if (!query) {
      cy.elements().removeClass('dimmed highlighted');
      return;
    }

    cy.batch(function () {
      cy.elements().addClass('dimmed').removeClass('highlighted');

      var matched = cy.nodes().filter(function (node) {
        var name = (node.data('name') || '').toLowerCase();
        var email = (node.data('email') || '').toLowerCase();
        return name.indexOf(query) >= 0 || email.indexOf(query) >= 0;
      });

      matched.removeClass('dimmed').addClass('highlighted');
      matched.connectedEdges().removeClass('dimmed');
      matched.neighborhood().nodes().removeClass('dimmed');
    });
  }

  function clearSearch() {
    if (!cy) return;
    cy.elements().removeClass('dimmed highlighted');
    var input = document.getElementById('graph-search');
    if (input) input.value = '';
  }

  // ---------------------------------------------------------------------------
  // Toolbar buttons
  // ---------------------------------------------------------------------------
  function setupToolbar() {
    var fitBtn = document.getElementById('btn-fit');
    var relayoutBtn = document.getElementById('btn-relayout');

    if (fitBtn) {
      fitBtn.addEventListener('click', function () {
        if (cy) cy.fit(undefined, 40);
      });
    }

    if (relayoutBtn) {
      relayoutBtn.addEventListener('click', function () {
        runLayout();
      });
    }
  }

  // ---------------------------------------------------------------------------
  // HTMX integration
  // ---------------------------------------------------------------------------
  function setupHtmxListeners() {
    // Re-fetch graph when CRUD operations complete
    document.body.addEventListener('graph-updated', function () {
      fetchGraph();
    });

    // Also listen for the theme toggle to re-apply styles
    var observer = new MutationObserver(function (mutations) {
      mutations.forEach(function (mutation) {
        if (mutation.attributeName === 'class' && cy) {
          cy.style(cyStyle());
        }
      });
    });
    observer.observe(document.documentElement, { attributes: true });
  }

  // ---------------------------------------------------------------------------
  // Init
  // ---------------------------------------------------------------------------
  function init() {
    initCy();
    setupSearch();
    setupToolbar();
    setupHtmxListeners();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
