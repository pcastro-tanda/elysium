status_var = :unprocessable_entity
head status_var
render json: { error: 'Invalid data' }, status: status_var
redirect_to some_path, status: status_var
assert_response status_var
