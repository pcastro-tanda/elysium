require 'spec_helper'
describe ArticlesController do
  render_views
    describe "GET 'index'" do
    ^^^^^^^^^^^^^^^^^^^^^^^^^ Inconsistent indentation detected.
            it "returns success" do
            end
        describe "admin user" do
        ^^^^^^^^^^^^^^^^^^^^^^^^ Inconsistent indentation detected.
             before(:each) do
            end
        end
    end
end
