x.hash || fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.hash` always returns a truthy value.
