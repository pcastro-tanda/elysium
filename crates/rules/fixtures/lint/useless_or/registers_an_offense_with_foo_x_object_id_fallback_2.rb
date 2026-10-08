(foo || x.object_id) || fallback
                     ^^^^^^^^^^^ `fallback` will never evaluate because `x.object_id` always returns a truthy value.
