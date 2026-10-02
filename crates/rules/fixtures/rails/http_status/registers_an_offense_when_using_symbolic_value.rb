render :foo, status: :ok
                     ^^^ Prefer `200` over `:ok` to define HTTP status code.
render json: { foo: 'bar' }, status: :not_found
                                     ^^^^^^^^^^ Prefer `404` over `:not_found` to define HTTP status code.
render status: :not_found, json: { foo: 'bar' }
               ^^^^^^^^^^ Prefer `404` over `:not_found` to define HTTP status code.
render plain: 'foo/bar', status: :not_modified
                                 ^^^^^^^^^^^^^ Prefer `304` over `:not_modified` to define HTTP status code.
redirect_to root_url, status: :moved_permanently
                              ^^^^^^^^^^^^^^^^^^ Prefer `301` over `:moved_permanently` to define HTTP status code.
redirect_to action: 'index', status: :moved_permanently
                                     ^^^^^^^^^^^^^^^^^^ Prefer `301` over `:moved_permanently` to define HTTP status code.
redirect_to root_path(utm_source: :pr, utm_medium: :web), status: :moved_permanently
                                                                  ^^^^^^^^^^^^^^^^^^ Prefer `301` over `:moved_permanently` to define HTTP status code.
head :ok
     ^^^ Prefer `200` over `:ok` to define HTTP status code.
head :ok, location: 'accounts'
     ^^^ Prefer `200` over `:ok` to define HTTP status code.
assert_response :ok
                ^^^ Prefer `200` over `:ok` to define HTTP status code.
assert_response :not_found, 'message'
                ^^^^^^^^^^ Prefer `404` over `:not_found` to define HTTP status code.
assert_redirected_to '/some/path', status: :moved_permanently
                                           ^^^^^^^^^^^^^^^^^^ Prefer `301` over `:moved_permanently` to define HTTP status code.
assert_redirected_to action: 'index', status: :moved_permanently
                                              ^^^^^^^^^^^^^^^^^^ Prefer `301` over `:moved_permanently` to define HTTP status code.
assert_redirected_to '/some/path', { status: :moved_permanently }, 'message'
                                             ^^^^^^^^^^^^^^^^^^ Prefer `301` over `:moved_permanently` to define HTTP status code.
