x.to_sym or fallback
         ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_sym` always returns a truthy value.
