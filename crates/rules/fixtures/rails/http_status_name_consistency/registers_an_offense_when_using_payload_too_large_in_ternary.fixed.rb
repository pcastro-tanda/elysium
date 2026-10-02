render json: { error: 'File too big' }, status: some_condition ? :content_too_large : :ok
