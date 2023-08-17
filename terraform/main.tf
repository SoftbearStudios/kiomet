module "game_terraform" {
    source = "../engine/game_terraform"

    name = "kiomet"
    domain = "kiomet.com"
    servers = {
        1 = "eu-central"
        2 = "us-east"
        3 = "eu-central"
        4 = "us-east"
    }
    aws_region = var.aws_region
    linode_token = var.linode_token
}

// From env.
variable "linode_token" {
    type = string
}

variable "aws_region" {
    type = string
    default = "us-east-1"
}

terraform {
    /*
    1) Manually create S3 bucket with name and region matching below.
    2) Manually create DynamoDB table with name and region matching below and primary key LockID (string)
    */
    backend "s3" {
        profile = "terraform"
        bucket = "softbear-terraform"
        key    = "kiomet.tfstate"
        dynamodb_table = "kiomet_terraform" // For locking.
        region = "us-east-1"
    }
}

terraform {
    required_providers {
        linode = {
            source  = "linode/linode"
            # version = "1.20.2"
        }
    }
}

provider "linode" {
    token = var.linode_token
}

provider "aws" {
    profile = "terraform"
    region = var.aws_region
}

data "aws_caller_identity" "current" {}