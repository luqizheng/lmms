export const BUTTON_VARIANTS = ["primary", "secondary", "ghost"] as const;
export type ButtonVariant = (typeof BUTTON_VARIANTS)[number];

export const BUTTON_SIZES = ["sm", "md", "lg", "icon"] as const;
export type ButtonSize = (typeof BUTTON_SIZES)[number];
