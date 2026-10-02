x.object_id || fallback || other_fallback
            ^^^^^^^^^^^ `fallback` will never evaluate because `x.object_id` always returns a truthy value.
