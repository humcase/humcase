// Origin UI Button, 21st id 143: Slot, CVA variants and props forwarding.
// https://21st.dev/@originui/components/button
// Humcase kit: design and states in button.css (single source), tokens of both themes.
import { Slot } from 'radix-ui';
import { cva, type VariantProps } from 'class-variance-authority';
import * as React from 'react';
import { cn } from '../../lib/utils';
import './button.css';

const buttonVariants = cva('ui-button', {
  variants: {
    variant: { default: 'ui-button--default', field: 'ui-button--field', destructive: 'ui-button--destructive', outline: 'ui-button--outline', secondary: 'ui-button--secondary', ghost: 'ui-button--ghost', link: 'ui-button--link', nav: 'ui-button--nav' },
    size: { default: 'ui-button--standard', xs: 'ui-button--xs', sm: 'ui-button--small', lg: 'ui-button--large', icon: 'ui-button--icon', 'icon-sm': 'ui-button--icon-sm' },
  },
  defaultVariants: { variant: 'default', size: 'default' },
});
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonVariants> { asChild?: boolean; }
const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = 'default', size = 'default', asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot.Root : 'button';
    return <Comp ref={ref} data-variant={variant} data-size={size} className={cn(buttonVariants({ variant, size, className }))} {...props} />;
  },
);
Button.displayName = 'Button';
export { Button };
