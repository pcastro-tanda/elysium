skip_before_action :login_required, except: :admin, if: :trusted_origin?
                                    ^^^^^^^^^^^^^^ `except` option will be ignored when `if` and `except` are used together.
