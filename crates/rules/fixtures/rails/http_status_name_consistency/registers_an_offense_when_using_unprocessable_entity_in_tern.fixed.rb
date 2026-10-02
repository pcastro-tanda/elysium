render json: { error: 'Invalid data' }, status: some_condition ? :unprocessable_content : :ok
