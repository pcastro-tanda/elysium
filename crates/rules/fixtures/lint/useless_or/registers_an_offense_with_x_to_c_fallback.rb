x.to_c || fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_c` always returns a truthy value.
