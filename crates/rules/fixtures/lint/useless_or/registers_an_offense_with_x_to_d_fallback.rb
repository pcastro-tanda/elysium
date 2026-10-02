x.to_d || fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_d` always returns a truthy value.
