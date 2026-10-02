x.to_a || fallback || other_fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_a` always returns a truthy value.
