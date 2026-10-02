render json: { error: 'Invalid data' }, status: some_condition ? :unprocessable_entity : :ok
                                                                 ^^^^^^^^^^^^^^^^^^^^^ Prefer `:unprocessable_content` over `:unprocessable_entity`.
