render json: { error: 'File too big' }, status: some_condition ? :payload_too_large : :ok
                                                                 ^^^^^^^^^^^^^^^^^^ Prefer `:content_too_large` over `:payload_too_large`.
