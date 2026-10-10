"use client"

import { Checkbox as CheckboxPrimitive } from "@base-ui/react/checkbox"
import { cn } from "cn"
import { CheckIcon } from "lucide-react"

function Checkbox({ className, ...props }: CheckboxPrimitive.Root.Props) {
  return (
    <CheckboxPrimitive.Root
      data-slot="checkbox"
      className={cn(
        "flex size-5 shrink-0 items-center justify-center rounded-md border-2 border-ink bg-white text-ink data-checked:bg-mint",
        className,
      )}
      {...props}
    />
  );
}

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