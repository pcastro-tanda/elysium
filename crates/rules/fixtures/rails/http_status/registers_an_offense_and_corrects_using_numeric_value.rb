render :foo, status: 200
                     ^^^ Prefer `:ok` over `200` to define HTTP status code.
render json: { foo: 'bar' }, status: 404
                                     ^^^ Prefer `:not_found` over `404` to define HTTP status code.
render status: 404, json: { foo: 'bar' }
               ^^^ Prefer `:not_found` over `404` to define HTTP status code.
render plain: 'foo/bar', status: 304
                                 ^^^ Prefer `:not_modified` over `304` to define HTTP status code.
redirect_to root_url, status: 301
                              ^^^ Prefer `:moved_permanently` over `301` to define HTTP status code.
redirect_to action: 'index', status: 301
                                     ^^^ Prefer `:moved_permanently` over `301` to define HTTP status code.
redirect_to root_path(utm_source: :pr, utm_medium: :web), status: 301
                                                                  ^^^ Prefer `:moved_permanently` over `301` to define HTTP status code.
head 200
     ^^^ Prefer `:ok` over `200` to define HTTP status code.
head 200, location: 'accounts'
     ^^^ Prefer `:ok` over `200` to define HTTP status code.
assert_response 200
                ^^^ Prefer `:ok` over `200` to define HTTP status code.
assert_response 404, 'message'
                ^^^ Prefer `:not_found` over `404` to define HTTP status code.
assert_redirected_to '/some/path', status: 301
                                           ^^^ Prefer `:moved_permanently` over `301` to define HTTP status code.
assert_redirected_to action: 'index', status: 301
                                              ^^^ Prefer `:moved_permanently` over `301` to define HTTP status code.
assert_redirected_to '/some/path', { status: 301 }, 'message'
                                             ^^^ Prefer `:moved_permanently` over `301` to define HTTP status code.
