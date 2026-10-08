skip_before_action :login_required, if: :trusted_origin?
