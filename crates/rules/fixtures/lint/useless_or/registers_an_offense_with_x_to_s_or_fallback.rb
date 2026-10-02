x.to_s or fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_s` always returns a truthy value.
