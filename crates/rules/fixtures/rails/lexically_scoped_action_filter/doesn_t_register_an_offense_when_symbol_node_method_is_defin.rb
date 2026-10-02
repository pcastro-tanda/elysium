class LoginController < ApplicationController
  skip_before_action :require_login, only: :health_check

  def health_check
  end
end
