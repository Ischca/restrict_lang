"use server";

import {
  evaluateQuote,
  type QuoteInput,
  type QuoteResult,
} from "@/lib/restrict.server";

export interface QuoteState {
  input: QuoteInput;
  quote: QuoteResult | null;
  error: string | null;
}

function readInteger(
  formData: FormData,
  name: string,
  minimum: number,
  maximum: number,
): number {
  const raw = formData.get(name);
  const value = typeof raw === "string" ? Number(raw) : Number.NaN;

  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new Error(`${name} must be an integer from ${minimum} to ${maximum}.`);
  }

  return value;
}

export async function calculateQuote(
  _previous: QuoteState,
  formData: FormData,
): Promise<QuoteState> {
  try {
    const input: QuoteInput = {
      unitPrice: readInteger(formData, "unitPrice", 1, 1_000_000),
      quantity: readInteger(formData, "quantity", 1, 1_000),
      member: formData.get("member") === "on",
    };

    return {
      input,
      quote: evaluateQuote(input),
      error: null,
    };
  } catch (error) {
    return {
      input: _previous.input,
      quote: _previous.quote,
      error: error instanceof Error ? error.message : "Quote calculation failed.",
    };
  }
}
