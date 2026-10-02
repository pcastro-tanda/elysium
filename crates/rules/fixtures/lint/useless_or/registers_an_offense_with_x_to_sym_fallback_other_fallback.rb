x.to_sym || fallback || other_fallback
         ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_sym` always returns a truthy value.
