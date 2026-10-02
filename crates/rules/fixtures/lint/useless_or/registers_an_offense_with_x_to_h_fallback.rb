x.to_h || fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_h` always returns a truthy value.
