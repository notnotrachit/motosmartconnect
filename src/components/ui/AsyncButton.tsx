import { useState, type MouseEvent } from "react";
import { Button, type ButtonProps } from "./Button";

interface AsyncButtonProps extends Omit<ButtonProps, "onClick" | "loading"> {
  onClick: (event: MouseEvent<HTMLButtonElement>) => Promise<unknown> | unknown;
}

/** Button that shows its own spinner until the click handler's promise settles. */
export function AsyncButton({ onClick, ...rest }: AsyncButtonProps) {
  const [loading, setLoading] = useState(false);
  return (
    <Button
      {...rest}
      loading={loading}
      onClick={async (event) => {
        setLoading(true);
        try {
          await onClick(event);
        } finally {
          setLoading(false);
        }
      }}
    />
  );
}
