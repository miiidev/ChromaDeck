"use client"

import { Checkbox as CheckboxPrimitive } from "@base-ui/react/checkbox"
import { cn } from "cn"
import { CheckIcon } from "lucide-react"

const Checkbox = CheckboxPrimitive.Root

function CheckboxIndicator({ className, ...props }: CheckboxPrimitive.Indicator.Props) {
  return (
    <CheckboxPrimitive.Indicator
      data-slot="checkbox-indicator"
      className={cn("flex size-4 items-center justify-center", className)}
      {...props}
    >
      <CheckIcon className="size-4" />
    </CheckboxPrimitive.Indicator>
  )
}

export { Checkbox, CheckboxIndicator }