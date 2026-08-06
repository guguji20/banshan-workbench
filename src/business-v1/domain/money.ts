import {
  assertSafeArithmetic,
  DomainValidationError,
} from "./validation";

export type CurrencyCode = "CNY";

export interface Money {
  readonly cents: number;
  readonly currency: CurrencyCode;
}

export function money(cents: number, currency: CurrencyCode = "CNY"): Money {
  if (!Number.isSafeInteger(cents)) {
    throw new DomainValidationError([
      {
        field: "money.cents",
        code: "safe_integer_required",
        message: "must be represented as integer cents",
      },
    ]);
  }
  return Object.freeze({ cents, currency });
}

export function addMoney(...values: readonly Money[]): Money {
  const currency = commonCurrency(values);
  return money(
    values.reduce(
      (total, value) => assertSafeArithmetic(total + value.cents, "money.total"),
      0,
    ),
    currency,
  );
}

export function subtractMoney(minuend: Money, subtrahend: Money): Money {
  assertSameCurrency(minuend, subtrahend);
  return money(
    assertSafeArithmetic(minuend.cents - subtrahend.cents, "money.difference"),
    minuend.currency,
  );
}

export function multiplyMoney(unitPrice: Money, quantity: number): Money {
  if (!Number.isSafeInteger(quantity) || quantity <= 0) {
    throw new DomainValidationError([
      {
        field: "quantity",
        code: "positive_safe_integer",
        message: "must be a positive safe integer",
      },
    ]);
  }
  return money(
    assertSafeArithmetic(unitPrice.cents * quantity, "money.lineTotal"),
    unitPrice.currency,
  );
}

export function percentageOf(base: Money, basisPoints: number): Money {
  if (!Number.isSafeInteger(basisPoints) || basisPoints < 0 || basisPoints > 10_000) {
    throw new DomainValidationError([
      {
        field: "basisPoints",
        code: "basis_points_range",
        message: "must be an integer between 0 and 10000",
      },
    ]);
  }
  const numerator = assertSafeArithmetic(base.cents * basisPoints, "money.percentage");
  return money(Math.round(numerator / 10_000), base.currency);
}

function commonCurrency(values: readonly Money[]): CurrencyCode {
  const currency = values[0]?.currency ?? "CNY";
  values.forEach((value) => {
    if (value.currency !== currency) {
      throw new DomainValidationError([
        {
          field: "money.currency",
          code: "currency_mismatch",
          message: "all monetary values must use the same currency",
        },
      ]);
    }
  });
  return currency;
}

function assertSameCurrency(left: Money, right: Money): void {
  commonCurrency([left, right]);
}
