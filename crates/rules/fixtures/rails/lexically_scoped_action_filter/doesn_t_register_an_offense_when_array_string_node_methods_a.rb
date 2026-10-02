class LoginController < ApplicationController
  before_action :require_login, only: %w[index settings]

  def index
  end

  def settings
  end
end
