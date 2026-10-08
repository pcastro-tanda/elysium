x.to_f || fallback || other_fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_f` always returns a truthy value.
