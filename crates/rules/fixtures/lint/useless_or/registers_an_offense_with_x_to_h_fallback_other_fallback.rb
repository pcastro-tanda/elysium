x.to_h || fallback || other_fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_h` always returns a truthy value.
