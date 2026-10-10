"use client"

import { RadioGroup as RadioGroupPrimitive } from "@base-ui/react/radio-group"
import { Radio as RadioPrimitive } from "@base-ui/react/radio"
import { cn } from "cn"

const RadioGroup = RadioGroupPrimitive

function RadioGroupItem({ className, ...props }: RadioPrimitive.Root.Props<unknown>) {
  return (
    <RadioPrimitive.Root
      data-slot="radio-group-item"
      className={cn("flex size-5 shrink-0 items-center justify-center rounded-full border-2 border-ink bg-white", className)}
      {...props}
    />
  )
}

function RadioGroupIndicator({ className, ...props }: RadioPrimitive.Indicator.Props) {
  return (
    <RadioPrimitive.Indicator
      data-slot="radio-group-indicator"
      className={cn("flex size-2 items-center justify-center rounded-full bg-ink", className)}
      {...props}
    />
  )
}

export { RadioGroup, RadioGroupIndicator, RadioGroupItem }