(function(global) {
  function highlightRestrictBlocks(root) {
    const highlighter = global.RestrictHighlight;
    if (!highlighter) {
      return;
    }

    if (global.hljs) {
      highlighter.registerHighlightJs(global.hljs);
      global.hljs.highlightAll();
      return;
    }

    const scope = root || document;
    scope.querySelectorAll('pre code.language-restrict').forEach((block) => {
      if (block.dataset.highlighted === 'yes') {
        return;
      }

      block.innerHTML = highlighter.highlightRestrict(block.textContent);
      block.dataset.highlighted = 'yes';
    });
  }

  global.RestrictCodeBlocks = {
    highlightRestrictBlocks
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => highlightRestrictBlocks(document));
  } else {
    highlightRestrictBlocks(document);
  }
})(window);
