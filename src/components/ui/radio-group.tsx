"use client"

import { RadioGroup as RadioGroupPrimitive } from "@base-ui/react/radio-group"
import { Radio as RadioPrimitive } from "@base-ui/react/radio"
import { cn } from "cn"

const RadioGroup = RadioGroupPrimitive

function RadioGroupItem({ className, ...props }: RadioPrimitive.Root.Props<unknown>) {
  return (
    <RadioPrimitive.Root
      data-slot="radio-group-item"
      className={cn(
        "flex size-5 shrink-0 items-center justify-center rounded-full border-2 border-white/20 bg-transparent data-checked:border-primary data-checked:bg-primary transition-[background-color,border-color] focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        className
      )}
      {...props}
    />
  )
}

function RadioGroupIndicator({ className, ...props }: RadioPrimitive.Indicator.Props) {
  return (
    <RadioPrimitive.Indicator
      data-slot="radio-group-indicator"
      className={cn("flex size-2.5 items-center justify-center rounded-full bg-primary-foreground", className)}
      {...props}
    />
  )
}

export { RadioGroup, RadioGroupIndicator, RadioGroupItem }