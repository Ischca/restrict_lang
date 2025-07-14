// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item affix "><a href="en/introduction.html">Introduction</a></li><li class="chapter-item affix "><li class="part-title">Getting Started</li><li class="chapter-item "><a href="en/getting-started/installation.html"><strong aria-hidden="true">1.</strong> Installation</a></li><li class="chapter-item "><a href="en/getting-started/hello-world.html"><strong aria-hidden="true">2.</strong> Hello, World!</a></li><li class="chapter-item affix "><li class="part-title">Language Guide</li><li class="chapter-item "><a href="en/guide/index.html"><strong aria-hidden="true">3.</strong> Overview</a></li><li class="chapter-item "><a href="en/guide/osv-order.html"><strong aria-hidden="true">4.</strong> OSV Word Order</a></li><li class="chapter-item "><a href="en/guide/syntax.html"><strong aria-hidden="true">5.</strong> Syntax Reference</a></li><li class="chapter-item "><a href="en/guide/types.html"><strong aria-hidden="true">6.</strong> Type System</a></li><li class="chapter-item "><a href="en/guide/ownership.html"><strong aria-hidden="true">7.</strong> Ownership &amp; Affine Types</a></li><li class="chapter-item "><a href="en/guide/patterns.html"><strong aria-hidden="true">8.</strong> Pattern Matching</a></li><li class="chapter-item "><a href="en/guide/warder.html"><strong aria-hidden="true">9.</strong> Warder Package Manager</a></li><li class="chapter-item affix "><li class="part-title">Reference</li><li class="chapter-item "><a href="en/reference/stdlib.html"><strong aria-hidden="true">10.</strong> Standard Library</a></li><li class="chapter-item "><a href="en/reference/keywords.html"><strong aria-hidden="true">11.</strong> Keywords</a></li><li class="chapter-item "><a href="en/reference/operators.html"><strong aria-hidden="true">12.</strong> Operators</a></li><li class="chapter-item affix "><li class="part-title">Advanced Topics</li><li class="chapter-item "><a href="en/advanced/wasm.html"><strong aria-hidden="true">13.</strong> WebAssembly Integration</a></li><li class="chapter-item "><a href="en/advanced/interop.html"><strong aria-hidden="true">14.</strong> Interoperability</a></li><li class="chapter-item "><a href="en/advanced/performance.html"><strong aria-hidden="true">15.</strong> Performance Tips</a></li><li class="chapter-item affix "><li class="spacer"></li><li class="chapter-item affix "><li class="part-title">日本語版</li><li class="chapter-item "><a href="ja/introduction.html"><strong aria-hidden="true">16.</strong> はじめに</a></li><li class="chapter-item "><a href="ja/getting-started/installation.html"><strong aria-hidden="true">17.</strong> インストール</a></li><li class="chapter-item "><a href="ja/getting-started/hello-world.html"><strong aria-hidden="true">18.</strong> Hello, World!</a></li><li class="chapter-item "><a href="ja/guide/syntax.html"><strong aria-hidden="true">19.</strong> 構文リファレンス</a></li><li class="chapter-item "><a href="ja/guide/types.html"><strong aria-hidden="true">20.</strong> 型システム</a></li><li class="chapter-item "><a href="ja/guide/warder.html"><strong aria-hidden="true">21.</strong> Warderパッケージマネージャー</a></li><li class="chapter-item "><a href="ja/reference/stdlib.html"><strong aria-hidden="true">22.</strong> 標準ライブラリ</a></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split("#")[0].split("?")[0];
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
