render :foo, status: 550
render json: { foo: bar }, status: 550
render plain: 'foo/bar', status: 550
redirect_to root_url, status: 550
redirect_to root_path(utm_source: :pr, utm_medium: :web), status: 550
head 550
assert_response 550
assert_response 550, 'message'
assert_redirected_to '/some/path', status: 550
assert_redirected_to action: 'index', status: 550
