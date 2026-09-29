"use client";

import { useActionState } from "react";

import { calculateQuote, type QuoteState } from "@/app/actions";

const yen = new Intl.NumberFormat("ja-JP", {
  style: "currency",
  currency: "JPY",
  maximumFractionDigits: 0,
});

export function QuoteCalculator({ initialState }: { initialState: QuoteState }) {
  const [state, action, pending] = useActionState(calculateQuote, initialState);

  return (
    <section className="calculator" aria-labelledby="calculator-title">
      <div className="section-heading">
        <p className="eyebrow">Server Function</p>
        <h2 id="calculator-title">Change the order</h2>
        <p>
          The form is interactive React. Submitting it calls the same Restrict
          Component on the Next.js server.
        </p>
      </div>

      <form action={action} className="quote-form">
        <label>
          <span>Unit price</span>
          <input
            name="unitPrice"
            type="number"
            min="1"
            max="1000000"
            defaultValue={state.input.unitPrice}
            required
          />
        </label>
        <label>
          <span>Quantity</span>
          <input
            name="quantity"
            type="number"
            min="1"
            max="1000"
            defaultValue={state.input.quantity}
            required
          />
        </label>
        <label className="member-field">
          <input
            name="member"
            type="checkbox"
            defaultChecked={state.input.member}
          />
          <span>Apply the 10% member discount</span>
        </label>
        <button type="submit" disabled={pending}>
          {pending ? "Calculating…" : "Calculate in Restrict"}
        </button>
      </form>

      {state.error ? <p className="error">{state.error}</p> : null}
      {state.quote ? (
        <dl className="receipt" aria-live="polite">
          <div>
            <dt>Subtotal</dt>
            <dd>{yen.format(state.quote.subtotal)}</dd>
          </div>
          <div>
            <dt>Member discount</dt>
            <dd>− {yen.format(state.quote.discount)}</dd>
          </div>
          <div>
            <dt>Shipping</dt>
            <dd>{yen.format(state.quote.shipping)}</dd>
          </div>
          <div className="total-row">
            <dt>Total</dt>
            <dd data-testid="quote-total">{yen.format(state.quote.total)}</dd>
          </div>
        </dl>
      ) : null}
    </section>
  );
}
