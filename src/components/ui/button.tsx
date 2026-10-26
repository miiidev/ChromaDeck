import { Button as ButtonPrimitive } from "@base-ui/react/button"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"

const buttonVariants = cva(
  "group/button inline-flex shrink-0 items-center justify-center rounded-[8px] border bg-clip-padding font-display text-[13px] font-semibold leading-none whitespace-nowrap transition-[translate,background-color,border-color] outline-none select-none focus-visible:ring-[3px] focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 [&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "border-transparent bg-primary text-primary-foreground hover:brightness-105 focus-visible:border-primary",
        outline:
          "border-border bg-transparent text-foreground hover:border-border-hover hover:bg-surface-hover aria-expanded:bg-surface-hover",
        secondary:
          "border-border bg-surface-hover text-foreground hover:bg-border-hover aria-expanded:bg-border-hover",
        ghost:
          "border-transparent bg-transparent text-foreground hover:bg-surface-hover aria-expanded:bg-surface-hover",
        destructive:
          "border-transparent bg-destructive text-destructive-foreground hover:brightness-105 focus-visible:border-destructive",
        link: "border-transparent text-foreground underline underline-offset-4 hover:underline",
      },
      size: {
        default: "h-11 gap-3 px-6",
        xs: "h-6 gap-1 px-3 text-xs",
        sm: "h-8 gap-1.5 px-4 text-xs",
        lg: "h-12 gap-3 px-8 text-sm",
        icon: "size-11",
        "icon-xs": "size-6",
        "icon-sm": "size-7",
        "icon-lg": "size-11",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

/**
 * Button-in-Button: a button with a trailing action gets its icon inside a
 * dedicated 32px circle, flush with the button's right inner padding.
 */
function Button({
  className,
  variant = "default",
  size = "default",
  children,
  trailingIcon,
  ...props
}: ButtonPrimitive.Props & VariantProps<typeof buttonVariants> & { trailingIcon?: React.ReactNode }) {
  const filled = variant === "default" || variant === "destructive"
  return (
    <ButtonPrimitive
      data-slot="button"
      className={cn(
        buttonVariants({ variant, size, className }),
        trailingIcon && "pl-6 pr-1.5",
      )}
      {...props}
    >
      <span className="btn-label inline-flex items-center gap-[inherit] relative z-10">
        {children}
      </span>
      {trailingIcon && (
        <span
          data-trailing-icon
          className={cn(
            "inline-flex size-8 shrink-0 items-center justify-center rounded-full transition-[translate]",
            filled ? "bg-primary-foreground/10" : "bg-surface-hover",
          )}
        >
          {trailingIcon}
        </span>
      )}
    </ButtonPrimitive>
  )
}

export { Button, buttonVariants }
