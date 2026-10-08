x.to_i || fallback || other_fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_i` always returns a truthy value.
