render :foo, status: 200
render json: { foo: bar }, status: 404
render plain: 'foo/bar', status: 304
redirect_to root_url, status: 301
redirect_to root_path(utm_source: :pr, utm_medium: :web), status: 301
head 200
assert_response 200
assert_response 404, 'message'
assert_redirected_to '/some/path', status: 301
assert_redirected_to action: 'index', status: 301
