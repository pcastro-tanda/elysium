class HomeController < ApplicationController
  def create
    messages = %w[foo bar baz]
    messages.each do |message|
      flash[:alert] = message
    end

    redirect_to :index
  end
end
