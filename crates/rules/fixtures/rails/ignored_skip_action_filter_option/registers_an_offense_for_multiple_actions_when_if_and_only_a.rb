skip_before_action :login_required, :another_action, only: :show, if: :trusted_origin?
                                                                  ^^^^^^^^^^^^^^^^^^^^ `if` option will be ignored when `only` and `if` are used together.
