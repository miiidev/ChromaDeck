import { Button as ButtonPrimitive } from "@base-ui/react/button"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"

const buttonVariants = cva(
  "group/button inline-flex shrink-0 items-center justify-center rounded-full border bg-clip-padding text-sm font-bold whitespace-nowrap transition-[translate,box-shadow,background-color,border-color] outline-none select-none focus-visible:ring-[3px] focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default:
          "border-transparent bg-primary text-primary-foreground hover:brightness-105 focus-visible:border-primary-border",
        outline:
          "border-border bg-surface text-foreground hover:bg-surface-hover aria-expanded:bg-surface-hover",
        secondary:
          "border-border bg-surface text-foreground hover:bg-surface-hover aria-expanded:bg-surface-hover",
        ghost:
          "border-transparent bg-transparent text-foreground hover:bg-surface-hover aria-expanded:bg-surface-hover",
        destructive:
          "border-transparent bg-destructive text-destructive-foreground hover:brightness-105 focus-visible:border-destructive",
        link: "text-foreground underline underline-offset-4 hover:underline",
      },
      size: {
        default:
          "h-11 gap-1.5 px-6 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2",
        xs: "h-6 gap-1 rounded-full px-3 text-xs in-data-[slot=button-group]:rounded-full has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3",
        sm: "h-8 gap-1 rounded-full px-4 text-xs in-data-[slot=button-group]:rounded-full has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3.5",
        lg: "h-12 gap-2 px-8 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2",
        icon: "size-11 rounded-full",
        "icon-xs": "size-6 rounded-full in-data-[slot=button-group]:rounded-full [&_svg:not([class*='size-'])]:size-3",
        "icon-sm": "size-7 rounded-full in-data-[slot=button-group]:rounded-full",
        "icon-lg": "size-11 rounded-full",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  children,
  trailingIcon,
  ...props
}: ButtonPrimitive.Props & VariantProps<typeof buttonVariants> & { trailingIcon?: React.ReactNode }) {
  return (
    <ButtonPrimitive
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    >
      <span className="btn-label inline-flex items-center gap-[inherit] relative z-10">
        {children}
      </span>
      {trailingIcon && (
        <span
          data-trailing-icon
          className="absolute right-2 z-10 inline-flex size-8 items-center justify-center rounded-full bg-black/5 dark:bg-white/10 transition-[translate,transform] group-hover/button:translate-x-1 group-hover/button:scale-105 group-active/button:translate-x-0 group-active/button:scale-100"
        >
          {trailingIcon}
        </span>
      )}
    </ButtonPrimitive>
  )
}

export { Button, buttonVariants }