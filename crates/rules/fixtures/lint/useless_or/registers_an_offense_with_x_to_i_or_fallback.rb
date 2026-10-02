x.to_i or fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_i` always returns a truthy value.
