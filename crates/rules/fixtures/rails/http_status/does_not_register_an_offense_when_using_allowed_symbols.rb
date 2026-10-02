render :foo, status: :error
render :foo, status: :success
render :foo, status: :missing
render :foo, status: :redirect
assert_response :error
assert_response :success
assert_response :missing
assert_response :redirect
