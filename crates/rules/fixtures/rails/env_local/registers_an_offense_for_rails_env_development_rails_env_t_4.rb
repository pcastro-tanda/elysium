!Rails.env.development? && !Rails.env.test? || foo?
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `!Rails.env.local?` instead.
foo? || !Rails.env.test? && !Rails.env.development?
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `!Rails.env.local?` instead.
