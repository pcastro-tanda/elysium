class LoginController < ApplicationController
  before_action :require_login, only: %w[index settings]
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `settings` is not explicitly defined on the class.

  def index
  end
end
