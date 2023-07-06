#include "bindings.h"

NVSDK_NGX_Resource_VK HELPERS_NVSDK_NGX_Create_ImageView_Resource_VK(
    VkImageView imageView,
    VkImage image,
    VkImageSubresourceRange subresourceRange,
    VkFormat format,
    unsigned int width,
    unsigned int height,
    bool readWrite) {

    return NVSDK_NGX_Create_ImageView_Resource_VK(
        imageView,
        image,
        subresourceRange,
        format,
        width,
        height,
        readWrite
    );
}

NVSDK_NGX_Result HELPERS_NGX_DLSS_GET_OPTIMAL_SETTINGS(
    NVSDK_NGX_Parameter *pInParams,
    unsigned int InUserSelectedWidth,
    unsigned int InUserSelectedHeight,
    NVSDK_NGX_PerfQuality_Value InPerfQualityValue,
    unsigned int *pOutRenderOptimalWidth,
    unsigned int *pOutRenderOptimalHeight,
    unsigned int *pOutRenderMaxWidth,
    unsigned int *pOutRenderMaxHeight,
    unsigned int *pOutRenderMinWidth,
    unsigned int *pOutRenderMinHeight,
    float *pOutSharpness) {

    return NGX_DLSS_GET_OPTIMAL_SETTINGS(
        pInParams,
        InUserSelectedWidth,
        InUserSelectedHeight,
        InPerfQualityValue,
        pOutRenderOptimalWidth,
        pOutRenderOptimalHeight,
        pOutRenderMaxWidth,
        pOutRenderMaxHeight,
        pOutRenderMinWidth,
        pOutRenderMinHeight,
        pOutSharpness
    );
}

NVSDK_NGX_Result HELPERS_NGX_VULKAN_CREATE_DLSS_EXT1(
    VkDevice InDevice,
    VkCommandBuffer InCmdList,
    unsigned int InCreationNodeMask,
    unsigned int InVisibilityNodeMask,
    NVSDK_NGX_Handle **ppOutHandle,
    NVSDK_NGX_Parameter *pInParams,
    NVSDK_NGX_DLSS_Create_Params *pInDlssCreateParams) {

    return NGX_VULKAN_CREATE_DLSS_EXT1(
        InDevice,
        InCmdList,
        InCreationNodeMask,
        InVisibilityNodeMask,
        ppOutHandle,
        pInParams,
        pInDlssCreateParams
    );
}

NVSDK_NGX_Result HELPERS_NGX_VULKAN_EVALUATE_DLSS_EXT(
    VkCommandBuffer InCmdList,
    NVSDK_NGX_Handle *pInHandle,
    NVSDK_NGX_Parameter *pInParams,
    NVSDK_NGX_VK_DLSS_Eval_Params *pInDlssEvalParams) {

    return NGX_VULKAN_EVALUATE_DLSS_EXT(
        InCmdList,
        pInHandle,
        pInParams,
        pInDlssEvalParams
    );
}
