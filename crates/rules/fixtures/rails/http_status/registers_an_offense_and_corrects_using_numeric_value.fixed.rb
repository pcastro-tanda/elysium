render :foo, status: :ok
render json: { foo: 'bar' }, status: :not_found
render status: :not_found, json: { foo: 'bar' }
render plain: 'foo/bar', status: :not_modified
redirect_to root_url, status: :moved_permanently
redirect_to action: 'index', status: :moved_permanently
redirect_to root_path(utm_source: :pr, utm_medium: :web), status: :moved_permanently
head :ok
head :ok, location: 'accounts'
assert_response :ok
assert_response :not_found, 'message'
assert_redirected_to '/some/path', status: :moved_permanently
assert_redirected_to action: 'index', status: :moved_permanently
assert_redirected_to '/some/path', { status: :moved_permanently }, 'message'
