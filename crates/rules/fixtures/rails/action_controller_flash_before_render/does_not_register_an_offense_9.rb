class HomeController < ApplicationController
  def create
    messages = %w[foo bar baz]
    messages.each { |message| flash[:alert] = message }

    redirect_to :index
  end
end
