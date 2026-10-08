class LoginController < ApplicationController
  skip_before_action :require_login, only: :health_check
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `health_check` is not explicitly defined on the class.

  def index
  end
end
