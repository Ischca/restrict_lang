import { QuoteCalculator } from "@/app/quote-calculator";
import { evaluateQuote, type QuoteInput } from "@/lib/restrict.server";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export default function Home() {
  const input: QuoteInput = {
    unitPrice: 1_200,
    quantity: 4,
    member: true,
  };
  const quote = evaluateQuote(input);

  return (
    <main>
      <header className="hero">
        <p className="eyebrow">Restrict × Next.js</p>
        <h1>Server-rendered UI, Wasm-native rules.</h1>
        <p className="lede">
          Next.js owns the page and interaction. Restrict owns the deterministic
          pricing rules behind a synchronous WebAssembly Component boundary.
        </p>
      </header>

      <div className="flow" aria-label="Execution boundary">
        <span>React UI</span>
        <b>→</b>
        <span>handwritten adapter</span>
        <b>→</b>
        <span>Restrict Component</span>
        <b>→</b>
        <span>pure domain package</span>
      </div>

      <section className="server-result" aria-labelledby="server-result-title">
        <div>
          <p className="eyebrow">React Server Component</p>
          <h2 id="server-result-title">Calculated before this page was sent</h2>
          <p>
            The initial total below was returned by the Restrict Component while
            this Server Component rendered. No Restrict runtime is shipped to
            the browser.
          </p>
        </div>
        <strong>¥{quote.total.toLocaleString("ja-JP")}</strong>
      </section>

      <QuoteCalculator
        initialState={{
          input,
          quote,
          error: null,
        }}
      />

      <aside className="boundary-note">
        <p className="eyebrow">Deliberate boundary</p>
        <p>
          The compiler emits a standard Component and WIT contract. The
          repository-local adapter chooses Next.js; neither Restrict syntax nor
          Warder contains React-specific behavior.
        </p>
      </aside>
    </main>
  );
}
