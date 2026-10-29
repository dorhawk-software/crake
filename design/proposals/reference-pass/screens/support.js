/*
 * Tiny stand-alone runtime for the Crake design screens.
 *
 * Each screen is an HTML file with an <x-dc> template ({{holes}}, <sc-for>,
 * <sc-if>, onClick="{{fn}}") and a <script type="text/x-dc"> that defines
 * `class Component extends DCLogic`. This file renders that template in a
 * plain browser, with no build step and no dependencies.
 *
 * Props: defaults come from the script's data-props. Override any of them in
 * the URL, e.g. tray-panel.html?dark=true
 */
(function () {
  'use strict';

  var render = null;

  function DCLogic(props) {
    this.props = props || {};
    this.state = {};
  }
  DCLogic.prototype.setState = function (patch) {
    if (typeof patch === 'function') patch = patch(this.state, this.props);
    this.state = Object.assign({}, this.state, patch);
    if (render) render();
  };
  DCLogic.prototype.forceUpdate = function () { if (render) render(); };
  window.DCLogic = DCLogic;

  var WHOLE = /^\s*\{\{([^}]+)\}\}\s*$/;

  function lookup(path, scopes) {
    path = path.trim();
    if (path === 'true') return true;
    if (path === 'false') return false;
    if (path === 'null') return null;
    if (/^-?\d+(\.\d+)?$/.test(path)) return Number(path);
    var parts = path.split('.');
    for (var i = scopes.length - 1; i >= 0; i--) {
      var s = scopes[i];
      if (s != null && Object.prototype.hasOwnProperty.call(s, parts[0])) {
        var v = s;
        for (var j = 0; j < parts.length; j++) {
          if (v == null) return undefined;
          v = v[parts[j]];
        }
        return v;
      }
    }
    return undefined;
  }

  function interp(str, scopes) {
    return str.replace(/\{\{([^}]+)\}\}/g, function (_, p) {
      var v = lookup(p, scopes);
      return v == null ? '' : String(v);
    });
  }

  function eventName(attr, el) {
    var name = attr.slice(2).toLowerCase();
    // React-style onChange on text fields fires on every keystroke.
    if (name === 'change' && el.localName === 'input' && !/^(checkbox|radio)$/.test(el.type)) return 'input';
    return name;
  }

  function build(node, scopes, parent) {
    if (node.nodeType === 3) {
      parent.appendChild(document.createTextNode(interp(node.nodeValue, scopes)));
      return;
    }
    if (node.nodeType !== 1) return;
    var tag = node.localName;

    if (tag === 'sc-for') {
      var lm = (node.getAttribute('list') || '').match(WHOLE);
      var list = (lm && lookup(lm[1], scopes)) || [];
      var as = node.getAttribute('as') || 'item';
      list.forEach(function (item, i) {
        var scope = { $index: i };
        scope[as] = item;
        node.childNodes.forEach(function (c) { build(c, scopes.concat([scope]), parent); });
      });
      return;
    }
    if (tag === 'sc-if') {
      var vm = (node.getAttribute('value') || '').match(WHOLE);
      if (vm && lookup(vm[1], scopes)) node.childNodes.forEach(function (c) { build(c, scopes, parent); });
      return;
    }

    var el = document.createElementNS(node.namespaceURI, node.localName);
    var valueProp = null;
    Array.prototype.forEach.call(node.attributes, function (a) {
      if (a.name.indexOf('hint-') === 0) return;
      var m = a.value.match(WHOLE);
      if (/^on[a-z]+$/i.test(a.name) && m) {
        var fn = lookup(m[1], scopes);
        if (typeof fn === 'function') el.addEventListener(eventName(a.name, el), fn);
        return;
      }
      var v = interp(a.value, scopes);
      if (a.name === 'value' && (tag === 'input' || tag === 'textarea')) valueProp = v;
      el.setAttribute(a.name, v);
    });
    node.childNodes.forEach(function (c) { build(c, scopes, el); });
    if (valueProp !== null) el.value = valueProp;
    parent.appendChild(el);
  }

  function boot() {
    var host = document.querySelector('x-dc');
    var script = document.querySelector('script[data-dc-script]');
    if (!host || !script) return;

    var helmet = host.querySelector('helmet');
    if (helmet) {
      Array.prototype.slice.call(helmet.children).forEach(function (n) { document.head.appendChild(n); });
      helmet.remove();
    }

    var template = document.createDocumentFragment();
    while (host.firstChild) template.appendChild(host.firstChild);
    host.style.display = 'block';

    var defs = {};
    try { defs = JSON.parse(script.getAttribute('data-props') || '{}'); } catch (e) { /* no props */ }
    var props = {};
    Object.keys(defs).forEach(function (k) {
      if (k.charAt(0) !== '$' && defs[k] && 'default' in defs[k]) props[k] = defs[k].default;
    });
    new URLSearchParams(location.search).forEach(function (v, k) {
      props[k] = v === 'true' ? true : v === 'false' ? false : (v !== '' && !isNaN(+v) ? +v : v);
    });

    var Component = new Function('DCLogic', script.textContent + '\nreturn Component;')(DCLogic);
    var inst = new Component(props);
    if (!inst.state) inst.state = {};
    var mounted = false;

    render = function () {
      var active = document.activeElement;
      var focusId = active && active.id;
      var selStart = null, selEnd = null;
      try { selStart = active.selectionStart; selEnd = active.selectionEnd; } catch (e) { /* not a text field */ }

      var vals = inst.renderVals ? inst.renderVals() : {};
      var out = document.createDocumentFragment();
      template.childNodes.forEach(function (n) { build(n, [vals], out); });
      host.replaceChildren(out);

      if (focusId) {
        var again = document.getElementById(focusId);
        if (again) {
          again.focus();
          try { if (selStart !== null) again.setSelectionRange(selStart, selEnd); } catch (e) { /* ignore */ }
        }
      }
      if (!mounted) {
        mounted = true;
        if (inst.componentDidMount) inst.componentDidMount();
      }
    };
    render();
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', boot);
  else boot();
})();
