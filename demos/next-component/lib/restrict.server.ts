import "server-only";

import { pricing } from "@/generated/restrict/restrict.js";
import type { QuoteResult } from "@/generated/restrict/interfaces/demo-restrict-next-demo-pricing.js";

export interface QuoteInput {
  unitPrice: number;
  quantity: number;
  member: boolean;
}

export type { QuoteResult };

/**
 * The only framework-facing adapter in the demo. Restrict and Warder know
 * nothing about React, RSC, Server Functions or FormData.
 */
export function evaluateQuote(input: QuoteInput): QuoteResult {
  return pricing.quote(input.unitPrice, input.quantity, input.member);
}
