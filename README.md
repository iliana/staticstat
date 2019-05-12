# staticstat

This is the Rube Goldberg machine I use to get hit counts for my static Netlify-hosted websites.

I haven't tested all the corner cases of the CloudFormation stack, and the execution policy on the log partitioning Lambda function is a bit wide. Please run this stack in its own AWS account ([AWS Organizations](https://aws.amazon.com/organizations/) helps you here), and **use at your own risk**! Although you can run this stack on smaller websites for pennies a month, do not blame me for your AWS bill.

You'll need to build the log partitioning function yourself; install Docker, `cd lambda-partition-log; ./build.sh`, and copy to S3. The S3 bucket and key are parameters to the stack.

Because the stack manipulates a CloudFront distribution, it can take half an hour to create, and must be created in `us-east-1`.
