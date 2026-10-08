arr.select(&:positive?)
arr.reject(&:positive?)
^^^^^^^^^^^^^^^^^^^^^^^ Use `partition` instead of consecutive `select` and `reject` calls.
